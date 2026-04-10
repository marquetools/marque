<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00030">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00030][Error] If ISM_USGOV_RESOURCE and attribute @ism:disseminationControls contains the name token [FOUO], 
        then attribute @ism:classification must have a value of [U].
        Human Readable: Portions marked for FOUO dissemination in a USA document
        must be classified UNCLASSIFIED.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_USGOV_RESOURCE, for each element which has 
    	attribute @ism:disseminationControls specified with a value containing
    	the token [FOUO] this rule ensures that attribute @ism:classification is 
    	specified with a value of [U].
    </sch:p>
    <sch:rule id="ISM-ID-00030-R1" context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('FOUO'))]">
        <sch:assert test="@ism:classification='U'" flag="error" role="error">
            [ISM-ID-00030][Error] If ISM_USGOV_RESOURCE and attribute @ism:disseminationControls contains the name token [FOUO], 
            then attribute @ism:classification must have a value of [U].
            Human Readable: Portions marked for FOUO dissemination in a USA document
            must be classified UNCLASSIFIED.
        </sch:assert>
    </sch:rule>
</sch:pattern>