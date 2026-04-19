<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?><?schematron-phases phaseids="BANNER VALUECHECK"?><!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       --><sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00496">
    <sch:p xmlns:ism="urn:us:gov:ic:ism"
          ism:classification="U"
          ism:ownerProducer="USA"
          class="ruleText">
        [ISM-ID-00496][Error] If @ism:compliesWith contains "USA-CUI-ONLY", 
        then there cannot be any other tokens in @ism:compliesWith.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism"
          ism:classification="U"
          ism:ownerProducer="USA"
          class="codeDesc">
    	If the document has @ism:compliesWith that contains "USA-CUI-ONLY", 
    	this rule ensures that there are no other tokens in @ism:compliesWith.
    </sch:p>
    <sch:rule id="ISM-ID-00496-R1"
             context="//*[contains(@ism:compliesWith,'USA-CUI-ONLY')]">
        <sch:assert test="./@ism:compliesWith='USA-CUI-ONLY'" flag="error" role="error">
            [ISM-ID-00496][Error] If @ism:compliesWith contains "USA-CUI-ONLY", 
            then there cannot be any other tokens in @ism:compliesWith.
        </sch:assert>
    </sch:rule>
</sch:pattern>
