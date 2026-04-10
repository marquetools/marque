<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLUP VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00347">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00347][Error] If ISM_USGOV_RESOURCE and if there exists a token in @ism:SARIdentifier for portions that contribute to
        rollup then they must also be specified in the @ism:SARIdentifier attribute on the ISM_RESOURCE_ELEMENT.
        
        Human Readable: All SAR Identifiers specified in the document that contribute to rollup must
        be rolled up to the resource level.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
       If ISM_USGOV_RESOURCE, match on the ISM_RESOURCE_ELEMENT if there are any @ism:SARIdentifier values specified on portions
       that are not @ism:excludeFromRollup="true" and then ensure that all the tokens found exist on the
       element are matched to. If there are any tokens not present in our element that exist elsewhere
       in the document's contributing portions, store them in the missingSAR variable. Then check
       that the missingSAR variable is empty or return an error message that specifies which tokens 
       are missing.
    </sch:p>
    <sch:rule id="ISM-ID-00347-R1" context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and count($partSARIdentifier_tok)&gt;0]">
        <!-- Check that all distinct tokens in SARIdentifier throughout the document that are not
            excludeFromRollup="true" are present in the SARIdentifier attribute of the
            ISM_RESOURCE_ELEMENT. If not return the missing token to the variable -->
        <sch:let name="missingSAR" value="for $token in distinct-values($partSARIdentifier) return if (index-of(tokenize(@ism:SARIdentifier,' '), $token) &gt; 0 ) then null else $token"/>
        <!-- check that the variable for missing SARIdentifier tokens is empty or error -->
        <sch:assert test="count($missingSAR)=0" flag="error" role="error">
            [ISM-ID-00347][Error] All SAR Identifiers specified in the document that contribute to rollup must
            be rolled up to the resource level. The following tokens were found to be missing from the resource
            element: <sch:value-of select="string-join($missingSAR, ', ')"/>.
        </sch:assert>
    </sch:rule>
</sch:pattern>