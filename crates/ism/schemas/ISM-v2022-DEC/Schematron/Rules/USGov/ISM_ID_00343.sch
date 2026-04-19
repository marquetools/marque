<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLUP VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00343">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00343][Error] If ISM_USGOV_RESOURCE and there exists a token in @ism:SCIcontrols for portions that contribute to
        rollup, then they must also be specified in the @ism:SCIcontrols attribute on the ISM_RESOURCE_ELEMENT.
        
        Human Readable: All SCI controls specified in the document that contribute to rollup must
        be rolled up to the resource level.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
       If the document is an ISM_USGOV_RESOURCE match on the ISM_RESOURCE_ELEMENT if there are any @ism:SCIcontrols 
       values specified on portions that are not @ism:excludeFromRollup="true" and then ensure that all the tokens found exist on the
       element are matched to. If there are any tokens not present in our element that exist elsewhere
       in the document's contributing portions, store them in the missingSCI variable. Then this rule ensures
       that the missingSCI variable is empty or return an error message that specifies which tokens 
       are missing.
    </sch:p>
    <sch:rule id="ISM-ID-00343-R1" context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and count($partSCIcontrols_tok)&gt;0]">
        <!-- Check that all distinct tokens in SCIcontrols throughout the document that are not
            excludeFromRollup="true" are present in the SCIcontrols attribute of the
            ISM_RESOURCE_ELEMENT. If not return the missing token to the variable -->
        <sch:let name="missingSCI" value="for $token in distinct-values($partSCIcontrols) return  if (index-of(tokenize(@ism:SCIcontrols,' '), $token) &gt; 0 ) then null else $token"/>
        <!-- check that the variable for missing SCIcontrol tokens is empty or error -->
        <sch:assert test="count($missingSCI)=0" flag="error" role="error">
            [ISM-ID-00343][Error] All SCI controls specified in the document that contribute to rollup must
            be rolled up to the resource level. The following tokens were found to be missing from the resource
            element: <sch:value-of select="string-join($missingSCI, ', ')"/>.
        </sch:assert>
    </sch:rule>
</sch:pattern>