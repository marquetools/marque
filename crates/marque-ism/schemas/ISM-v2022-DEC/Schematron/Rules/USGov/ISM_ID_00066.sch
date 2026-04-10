<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLUP VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00066">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00066][Error] If ISM_USGOV_RESOURCE and: 
        1. Any element meeting ISM_CONTRIBUTES in the document has the attribute @ism:disseminationControls containing [FOUO]
        AND
        2. ISM_RESOURCE_ELEMENT has the attribute @ism:classification [U]
        AND
        3. No element meeting ISM_CONTRIBUTES in the document has @ism:nonICmarkings
        AND
        4. Elements meeting ISM_CONTRIBUTES only contain dissemination controls 
        [REL], [RELIDO],[NF], [DISPLAYONLY], [EYES], and [FOUO].
        
        Then the ISM_RESOURCE_ELEMENT must have @ism:disseminationControls containing [FOUO].
        
        Human Readable: USA Unclassified documents having FOUO data, no non IC Markings, and only 
        contains dissemination controls [REL], [RELIDO], [NF], [DISPLAYONLY], [EYES], and [FOUO] must have 
        FOUO at the resource level.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_USGOV_RESOURCE, the current element is the ISM_RESOURCE_ELEMENT,
        some element meeting ISM_CONTRIBUTES specifies attribute @ism:disseminationControls
        with a value containing [FOUO], the ISM_RESOURCE_ELEMENT specifies the attribute
        @ism:classification with a value of [U], no element meeting ISM_CONTRIBUTES
        specifies attribute @ism:nonICmarkings, and elements meeting ISM_CONTRIBUTES
        only contain @ism:disseminationControls with tokens [REL], [RELIDO], [NF], [DISPLAYONLY], [EYES], and [FOUO], 
        then the resource element must contain @ism:disseminationControls with a value containing the token [FOUO].
    </sch:p>
    <sch:rule id="ISM-ID-00066-R1" context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and index-of($dcTagsFound,'FOUO') &gt; 0 and util:containsAnyOfTheTokens(@ism:classification, ('U')) and count($partNonICmarkings_tok) = 0 and util:containsOnlyTheTokens(string-join($partDisseminationControls, ' '), ('REL', 'RELIDO', 'NF', 'EYES', 'DISPLAYONLY', 'FOUO'))]">
        <sch:assert test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('FOUO'))" flag="error" role="error">
            [ISM-ID-00066][Error] If ISM_USGOV_RESOURCE and: 
            1. Any element meeting ISM_CONTRIBUTES in the document has the attribute @ism:disseminationControls containing [FOUO]
            AND
            2. ISM_RESOURCE_ELEMENT has the attribute @ism:classification [U]
            AND
            3. No element meeting ISM_CONTRIBUTES in the document has @ism:nonICmarkings
            AND
            4. Elements meeting ISM_CONTRIBUTES only contain dissemination controls 
            [REL], [RELIDO],[NF], [DISPLAYONLY], [EYES], and [FOUO].
            
            Then the ISM_RESOURCE_ELEMENT must have @ism:disseminationControls containing [FOUO].
            
            Human Readable: USA Unclassified documents having FOUO data, no non IC Markings, and only 
            contains dissemination controls [REL], [RELIDO], [NF], [DISPLAYONLY], [EYES], and [FOUO] must have 
            FOUO at the resource level.
        </sch:assert>
    </sch:rule>
</sch:pattern>