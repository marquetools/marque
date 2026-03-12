<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?><?schematron-phases phaseids="ROLLUP VALUECHECK"?><!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       --><sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00502">
    <sch:p xmlns:ism="urn:us:gov:ic:ism"
          ism:classification="U"
          ism:ownerProducer="USA"
          class="ruleText">
        [ISM-ID-00502][Error] If ISM_USCUI_RESOURCE or ISM_USCUIONLY_RESOURCE, and
        there exists a token in @ism:cuiBasic for portions that contribute to rollup, then all such tokens must
        also be specified in the @ism:cuiBasic attribute on the ISM_RESOURCE_ELEMENT. 
        
        Human Readable: All CUI Basic category markings specified in the document that contribute to
        rollup must be rolled up to the resource level. 
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism"
          ism:classification="U"
          ism:ownerProducer="USA"
          class="codeDesc"> If the document is an ISM_USCUI_RESOURCE or ISM_USCUIONLY_RESOURCE, match on
        the ISM_RESOURCE_ELEMENT. If there are any @ism:cuiBasic values specified on portions that
        are not @ism:excludeFromRollup="true", then ensure that all the tokens found exist on the matched resource
        element. If there are any tokens not present in the matched resource element that exist
        elsewhere in the document's contributing portions, store them in the missingCuiBasic variable.
        Then this rule ensures that the missingCuiBasic variable is empty or else return an error message that
        specifies which tokens are missing. 
    </sch:p>
    <sch:rule id="ISM-ID-00502-R1"
             context="*[($ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE) and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and count($partCuiBasic_tok) &gt; 0]">
        <!-- Check that all distinct tokens in @ism:cuiBasic throughout the document that are not
            @ism:excludeFromRollup="true" are present in the @ism:cuiBasic attribute of the
            ISM_RESOURCE_ELEMENT. If not, then return the missing token to the variable -->
        <sch:let name="missingCuiBasic"
               value="for $token in distinct-values($partCuiBasic) return if (index-of(tokenize(@ism:cuiBasic, ' '), $token) &gt; 0) then null else  $token"/>

        <!-- check that the variable for missing ism:cuiBasic tokens is empty or error -->
        <sch:assert test="count($missingCuiBasic) = 0" flag="error" role="error"> 
            [ISM-ID-00502][Error] All CUI Basic category markings specified in the document that contribute to rollup must be rolled up
            to the resource level. The following tokens were found to be missing from the resource
            element: <sch:value-of select="string-join($missingCuiBasic, ', ')"/>.
        </sch:assert>
    </sch:rule>
</sch:pattern>
