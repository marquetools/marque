<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00328">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00328][Error] If ISM_USGOV_RESOURCE and: 
        1. Any element in the document that has the attribute @ism:disseminationControls containing [FOUO]
        AND
        2. Has the attribute @ism:classification [U]
        
        Then the element can't have any @ism:nonICMarkings.
        
        Human Readable: Non-IC dissemination control markings in elements of USA Unclassified documents 
        supersede and take precedence over FOUO.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_USGOV_RESOURCE, for any element that contains @ism:disseminationControls
        with a value containing [FOUO] and has @ism:classification with a value of [U], 
        then this rule ensures that there is no @ism:nonICMarkings.
    </sch:p>
    <sch:rule id="ISM-ID-00328-R1" context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('FOUO')) and util:containsAnyOfTheTokens(@ism:classification, ('U'))]">
        <sch:assert test="not(@ism:nonICmarkings)" flag="error" role="error">
            [ISM-ID-00328][Error] If ISM_USGOV_RESOURCE and: 
            1. Any element in the document that has the attribute @ism:disseminationControls containing [FOUO]
            AND
            2. Has the attribute @ism:classification [U]
            
            Then the element can't have any @ism:nonICMarkings.
            
            Human Readable: Non-IC dissemination control markings in elements of USA Unclassified documents 
            supersede and take precedence over FOUO.
        </sch:assert>
    </sch:rule>
</sch:pattern>