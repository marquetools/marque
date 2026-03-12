<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00031">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00031][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:disseminationControls contains the name token [REL] or [EYES], then 
        attribute @ism:releasableTo must be specified. 
        Human Readable: USA documents containing REL TO or EYES ONLY 
        dissemination must specify to which countries the document is releasable.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_USGOV_RESOURCE, for each element which has 
    	attribute @ism:disseminationControls specified with a value containing
    	the token [REL] or [EYES] this rule ensures that attribute @ism:releasableTo
    	is specified.
    </sch:p>
    <sch:rule id="ISM-ID-00031-R1" context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('REL', 'EYES'))]">
        <sch:assert test="@ism:releasableTo" flag="error" role="error">
            [ISM-ID-00031][Error] If ISM_USGOV_RESOURCE and attribute 
            @ism:disseminationControls contains the name token [REL] or [EYES], then 
            attribute @ism:releasableTo must be specified. 
            Human Readable: USA documents containing REL TO or EYES ONLY 
            dissemination must specify to which countries the document is releasable.
        </sch:assert>
    </sch:rule>
</sch:pattern>